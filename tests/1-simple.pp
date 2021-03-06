#define(h){Hello};
#define(w){world};
#assert(simple)(Hello world!)(#h #w!)

#define(x){#h #w};
#assert(indir)(Hello world)(#x)

#define(z y:str){#y world};
#define(abc y:str){#y world};
#assert(param)(Hello world)(#z(Hello))

#define(q y:closure){#h: #expand(#rescope(#y){#w} ) #w };
#define(w){new world order};
#assert(scope)(Hello: new world order world )(#q{XYZ})

#assert(change_char)(Hello new world order)(#change_char(@)(#literal{#}){@h} #w)

#define(comment x:any){};

#comment{
TODO - Fix the below test; it was failing for an unknown reason.
#assert(change_par)(#literal{

world  world
:)
})(
#change_char(@)(#literal{#});


	@z(#change_char(@)(#end_paren); world @
		#change_char(@)(#literal{)}); #literal{:@}
	)
}

#define(recur x:str){#if_eq(#x)(yikes){#x, hello}{oh #recur(yikes)}};

#assert(recur)
	(oh yikes, hello)
	(#recur(here))
#assert(recur_2)
	(oh yikes, hello)
	(#recur(there))

#if_eq_then(#h)(Yolo){ERR1}{#define(w){as a test:}; #define(z){hello world};};

#assert(if_eq_then)
	(as a test: hello world)
	(#w #z)

#define(X){X};
#define(Y){Y};

#assert(list)(#list(
	#X
	#Y
))(#list(
	#literal{X}
	#literal{Y}
))

#assert(regex)(#list(
	#literal{hello world}
	#literal{el}
	#literal{lo}
))(#match_regex
	(h(el)(lo).*)
	(hello world)
)

#define(my_list)(#list(#literal{Welcome}));

#assert(head)
	(Welcome)
	(#head(#my_list))

#assert(tail)
	(#list( #literal{2} #literal{3} ))
	(#tail(#list( #literal{1} #literal{2} #literal{3} )))

#assert(join)
	(#list( #literal{Welcome} #literal{2} #literal{3} ))
	(#join(#my_list)(#list(#literal{2} #literal{3})))

#define(bool str:string){
	#if_eq(#str)(true){
		#tag(1)
	}{
		#tag(0)
	}
};

#define(str_of_bool b:bool){
	#if_eq(#b)(#bool(true)) {yes}{no}
};

#define(str_of_list l:list){
	#if_eq(#l)(#list()){}
		{#head(#l) #str_of_list(#tail(#l))}
};

#assert(type)
	(#bool(true))
	(#bool(true))

#assert(type removal)
	(#untag{bool}(#bool(true)))
	(1)

#assert(str_of_bool)
	(yes no)
	(#str_of_bool(#bool(true)) #str_of_bool(#bool(false)))

#assert(str_of_list)
	(a b c )
	(#str_of_list(#list(#literal{a} #literal{b} #literal{c})))
