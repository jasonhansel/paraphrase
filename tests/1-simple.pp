#define(h){Hello};
#define(w){world};
#assert(simple)(Hello world!)(#h #w!)

#define(x){#h #w};
#assert(indir)(Hello world)(#x)

#define(z :y){#y world};
#define(abc :y){#y world};
#assert(param)(Hello world)(#z(Hello))

#define(q :y){#h: #expand(#rescope(#y){#w} ) #w };
#define(w){new world order};
#assert(scope)(Hello: new world order world )(#q{XYZ})

#assert(change_char)(Hello new world order)(#change_char(@)(#literal{#}){@h} #w)

#assert(change_par)(#literal{

 world  world
 :)
})(
#change_char(@)(#literal{#});
@z(#change_char(@)(#end_paren); world @
#change_char(@)(#literal{)}); #literal{:@}
)

#define(recur :x){#if_eq(#x)(yikes){#x, hello}{oh #recur(yikes)}};

#assert(recur)(oh yikes, hello)(#recur(here))
#assert(recur_2)(oh yikes, hello)(#recur(there))

#if_eq_then(#h)(Yolo){ERR1}{#define(w){as a test:}; #define(z){hello world};};

#assert(if_eq_then)(as a test: hello world)(#w #z)


#assert(list)(XY)(#list(
	#literal{X}
	#literal{Y}
))
