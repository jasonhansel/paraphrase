
#define(bit str:string){
	#if_eq(#str)(0){
		#tag(0)
	}{
		#tag(1)
	}
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

#define(sum a:num b:num c:bit){
	#define(a)(#untag{num}(#a));
	#define(b)(#untag{num}(#b));
	#if_eq(#a)(#list()) { #num(#b) }{
	#if_eq(#b)(#list()) { #num(#a) }{
	#define(isum)(#sum_3(#head(#a))(#head(#b))(#c));
	#num(#list(
		#head(#isum) #untag{num}(#sum(#tail(#a))(#tail(#b))( #tail(#isum))  )
	))
	}}
};

#untag{num}(#sum
	(#num(#list(#bit(1) #bit(1) #bit(0))))
	(#num(#list(#bit(0) #bit(1) #bit(1))))
	(#bit(0))
)
