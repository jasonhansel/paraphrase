P1
#define(bit str:string){
	#tag(#str)
};

#define(num bits:list){
	#tag(#bits)
};

#define(sum_2 a:bit b:bit) {
	#if_eq(#a)(#bit(1)){
		#if_eq(#b)(#bit(1)){
			#list( #bit(0) #bit(1) )
		}{
			#list( #bit(1) #bit(0) )
		}
	}{
		#list(#b #bit(0))
	}
};

#define(sum_3 a:bit b:bit c:bit) {
	#if_eq(#c)(#bit(0)){
		#sum_2(#a)(#b)
	}{
		#define(inner)(#sum_2(#a)(#b)){
			#if_eq( #head(#inner) )(#bit(0)) {
				#list( #bit(1) #head(#tail(#inner)))
			}{
				#list(#bit(0) #bit(1))
			}
		}
	}
};

#define(sum_carry a:num b:num c:bit){
	#define(a)(#untag{num}(#a));
	#define(b)(#untag{num}(#b));
	#if_eq(#a)(#list()) {
		#if_eq(#c)(#bit(1)){ #sum_carry(#num(#b))(#num(#list(#bit(0))))(#c) }{ #num(#b) }
	}{
	#if_eq(#b)(#list()) {
		#if_eq(#c)(#bit(1)){ #sum_carry(#num(#a))(#num(#list(#bit(0))))(#c) }{ #num(#a) }
	}{
	#define(isum)(#sum_3(#head(#a))(#head(#b))(#c));
	#num(
	#join
		(#list(#head(#isum)))
		(#untag{num}(
			#sum_carry(#num(#tail(#a)))(#num(#tail(#b)))( #head(#tail(#isum)))
		))
	)
	}}
};

#define(sum a:num b:num){ #sum_carry(#a)(#b)(#bit(0)) };

#define(pad_num n:num w:num) {
	#define(bits)(#untag{num}(#n));
	#if_eq(#untag{num}(#w))(#list(#bit(0) #bit(0) #bit(0) #bit(1) )) { #list() }{
	#if_eq(#bits)(#list()){
		#join
			(#list(#bit(0)))
			(#pad_num(#num(#bits))(#sum(#w)(#num(#list(#bit(1))))))
	}{
	#join
		(#pad_num(#num(#tail(#bits)))(#sum(#w)(#num(#list(#bit(1))))))
		(#list(#head(#bits)))
	}}
};

#define(pretty_list l:list) {#if_eq(#l)(#list()){ };#untag{bit}(#head(#l)) #pretty_list(#tail(#l))};

#define(count_up n:num){
	#if_eq(#untag{num}(#n))(#list(#bit(0) #bit(0) #bit(0) #bit(0) 
	                              #bit(0) #bit(0) #bit(0) #bit(0) #bit(1))) {  }{
		#pretty_list(
			#pad_num
				(#n)
				(#num(#list(#bit(0)))))
		#count_up(
			#sum
				(#n)
				(#num(#list(#bit(1))))
		)
	}
};
8 256
#count_up(#num(#list(#bit(0))))

